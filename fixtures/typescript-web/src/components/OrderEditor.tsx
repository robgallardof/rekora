import { saveOrder, type OrderDto } from "../api/contract";

export function OrderEditor(props: { order: OrderDto }) {
  return <button onClick={() => saveOrder(props.order)}>Save</button>;
}
