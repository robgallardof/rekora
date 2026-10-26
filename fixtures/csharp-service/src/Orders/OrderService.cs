namespace Shop.Orders;

/// <summary>Coordinates order updates with optimistic concurrency.</summary>
public sealed class OrderService
{
    private readonly IOrderRepository _repository;
    private readonly BusinessScope _scope;

    public OrderService(IOrderRepository repository, BusinessScope scope)
    {
        _repository = repository;
        _scope = scope;
    }

    public async Task<Order> UpdateLineAsync(Guid orderId, OrderLine line, byte[] rowVersion)
    {
        var order = await _repository.GetAsync(orderId, _scope);
        order.ReplaceLine(line);
        await _repository.SaveAsync(order, rowVersion);
        return order;
    }
}
